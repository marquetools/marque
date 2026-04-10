<?xml version="1.0" encoding="UTF-8"?>
<?ICEA pattern?>
<?schematron-phases phaseids="BANNER PORTION VALUECHECK"?>
<!-- Notices - Distribution Notice: 
           This document has been approved for Public Release and is available for use without restriction.
       -->
<sch:pattern xmlns:sch="http://purl.oclc.org/dsdl/schematron" id="ISM-ID-00204" is-a="AttributeValueDeprecatedWarning">
    <sch:p xmlns:ism="urn:us:gov:ic:ism" ism:classification="U" ism:ownerProducer="USA" class="ruleText">
    	[ISM-ID-00204][Warning] Attribute @ism:SCIcontrols should not contain any values which will be deprecated.
    </sch:p>
    <sch:p xmlns:ism="urn:us:gov:ic:ism" ism:classification="U" ism:ownerProducer="USA" class="codeDesc">
      For each element which specifies attribute @ism:SCIcontrols,
      this rule ensures that the value of @ism:SCIcontrols has not been deprecated. 
      This is indicated in the CVE file by an attribute (@deprecated) 
      on the term element for that SCIcontrols value. 
      If the current date is less than or equal to the date value in (@deprecated), 
      then a deprecation warning will be given.
    </sch:p>
      <sch:param name="ruleId" value="'ISM-ID-00204'"/>
	<sch:param name="context" value="*[@ism:SCIcontrols]"/>
	<sch:param name="attrName" value="SCIcontrols"/>
	<sch:param name="cveName" value="CVEnumISMSCIControls"/>
	<sch:param name="cveSpec" value="ISM"/>
</sch:pattern>