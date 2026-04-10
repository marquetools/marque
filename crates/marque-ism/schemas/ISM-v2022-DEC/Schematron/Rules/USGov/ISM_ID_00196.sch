<?xml version="1.0" encoding="UTF-8"?>
<?ICEA pattern?>
<?schematron-phases phaseids="BANNER PORTION VALUECHECK"?>
<!-- Notices - Distribution Notice: 
           This document has been approved for Public Release and is available for use without restriction.
       -->
<sch:pattern xmlns:sch="http://purl.oclc.org/dsdl/schematron" id="ISM-ID-00196" is-a="AttributeValueDeprecatedWarning">
    <sch:p xmlns:ism="urn:us:gov:ic:ism" ism:classification="U" ism:ownerProducer="USA" class="ruleText">
    	[ISM-ID-00196][Warning] Attribute @ism:ownerProducer should not contain any values which will be deprecated.
    </sch:p>
    <sch:p xmlns:ism="urn:us:gov:ic:ism" ism:classification="U" ism:ownerProducer="USA" class="codeDesc">
      For each element which specifies attribute @ism:ownerProducer,
      this rule ensures that the value of @ism:ownerProducer has not been deprecated. 
      This is indicated in the CVE file by an attribute (@deprecated) 
      on the term element for that ownerProducer value. 
      If the current date is less than or equal to the date value in (@deprecated), 
      then a deprecation warning will be given.
    </sch:p>
      <sch:param name="ruleId" value="'ISM-ID-00196'"/>
	<sch:param name="context" value="*[@ism:ownerProducer]"/>
	<sch:param name="attrName" value="ownerProducer"/>
	<sch:param name="cveName" value="CVEnumISMCATOwnerProducer"/>
	<sch:param name="cveSpec" value="ISMCAT"/>
</sch:pattern>