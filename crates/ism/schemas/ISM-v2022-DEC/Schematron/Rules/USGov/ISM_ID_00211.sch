<?xml version="1.0" encoding="UTF-8"?>
<?ICEA pattern?>
<?schematron-phases phaseids="BANNER PORTION VALUECHECK"?>
<!-- Notices - Distribution Notice: 
           This document has been approved for Public Release and is available for use without restriction.
       -->
<sch:pattern xmlns:sch="http://purl.oclc.org/dsdl/schematron" id="ISM-ID-00211" is-a="AttributeValueDeprecatedError">
    <sch:p xmlns:ism="urn:us:gov:ic:ism" ism:classification="U" ism:ownerProducer="USA" class="ruleText">
    	[ISM-ID-00211][Error] Attribute @ism:nonUSControls must not contain values which have passed their deprecation date.
    </sch:p>
    <sch:p xmlns:ism="urn:us:gov:ic:ism" ism:classification="U" ism:ownerProducer="USA" class="codeDesc">
      For each element which specifies attribute @ism:nonUSControls,
      this rule ensures that the value of @ism:nonUSControls has not been deprecated. 
      This is indicated in the CVE file by an attribute (@deprecated) 
      on the term element for that nonUSControls value. 
      If the current date is greater than the date value in (@deprecated), 
      then a deprecation error will be given.
    </sch:p>
	  <sch:param name="ruleId" value="'ISM-ID-00211'"/>
	  <sch:param name="context" value="*[@ism:nonUSControls]"/>
	  <sch:param name="attrName" value="nonUSControls"/>
	  <sch:param name="cveName" value="CVEnumISMNonUSControls"/>
	  <sch:param name="cveSpec" value="ISM"/>
</sch:pattern>