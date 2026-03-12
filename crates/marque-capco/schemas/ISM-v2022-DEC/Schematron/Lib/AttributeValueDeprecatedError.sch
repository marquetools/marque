<?xml version="1.0" encoding="UTF-8"?>
<!-- Notices - Distribution Notice: 
           This document has been approved for Public Release and is available for use without restriction.
       -->
<sch:pattern xmlns:sch="http://purl.oclc.org/dsdl/schematron" abstract="true" id="AttributeValueDeprecatedError">
	<sch:p xmlns:ism="urn:us:gov:ic:ism" ism:classification="U" ism:ownerProducer="USA" class="codeDesc">
		Abstract pattern to ensure that an attribute does not contain a deprecated token. 
		This pattern uses the deprecation dates in the CVE passed from the calling rule and 
		the ISM_RESOURCE_CREATE_DATE to determine if a token in the attribute is deprecated, 
		which is an error. The context, CVE name, and Spec name are passed from the calling rule.</sch:p>
	<sch:rule id="AttributeValueDeprecatedError-R1" context="$context">
		<sch:assert test="count( dvf:deprecated( string(@ism:$attrName), document('../../CVE/$cveSpec/$cveName.xml')//cve:CVE/cve:Enumeration/cve:Term[./@deprecated], $ISM_RESOURCE_CREATE_DATE, true()) )=0" flag="error" role="error"> [<sch:value-of select="$ruleId"/>][Error] For attribute <sch:value-of select="'$attrName'"/>, value(s) <sch:value-of select="dvf:deprecated( string(@ism:$attrName), document('../../CVE/$cveSpec/$cveName.xml')//cve:CVE/cve:Enumeration/cve:Term[@deprecated], $ISM_RESOURCE_CREATE_DATE, true())"/>
		</sch:assert>
	</sch:rule>
</sch:pattern>