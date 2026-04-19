<?xml version="1.0" encoding="UTF-8"?>
<!-- Notices - Distribution Notice: 
           This document has been approved for Public Release and is available for use without restriction.
       -->
<sch:pattern xmlns:sch="http://purl.oclc.org/dsdl/schematron" abstract="true" id="AttributeValueDeprecatedWarning">
	<sch:p xmlns:ism="urn:us:gov:ic:ism" ism:classification="U" ism:ownerProducer="USA" class="codeDesc">
		Abstract pattern used to warn that an attribute value has a deprecation
		date in its CVE but has not passed based on the ISM_RESOURCE_CREATE_DATE of the resource.
		This pattern uses the deprecation dates in the CVE passed from the calling rule and the
		ISM_RESOURCE_CREATE_DATE to determine if a token in the attribute has a depreciation date,
		which is a warning. The context, CVE name, and Spec name are passed from the calling
		rule.</sch:p>
	<sch:rule id="AttributeValueDeprecatedWarning-R1" context="$context">
		<sch:assert test="count( dvf:deprecated( string(@ism:$attrName), document('../../CVE/$cveSpec/$cveName.xml')//cve:CVE/cve:Enumeration/cve:Term[./@deprecated], $ISM_RESOURCE_CREATE_DATE, false()))=0" flag="warning" role="warning"> [<sch:value-of select="$ruleId"/>][Warning] For attribute <sch:value-of select="'$attrName'"/>, value(s) <sch:value-of select="dvf:deprecated(string(@ism:$attrName), document('../../CVE/$cveSpec/$cveName.xml')//cve:CVE/cve:Enumeration/cve:Term[@deprecated], $ISM_RESOURCE_CREATE_DATE,false())"/>
		</sch:assert>
	</sch:rule>
</sch:pattern>