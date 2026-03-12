<?xml version="1.0" encoding="UTF-8"?>
<?ICEA pattern?>
<?schematron-phases phaseids="BANNER PORTION VALUECHECK"?>
<!-- Notices - Distribution Notice: 
           This document has been approved for Public Release and is available for use without restriction.
       -->
<sch:pattern xmlns:sch="http://purl.oclc.org/dsdl/schematron" id="ISM-ID-00198" is-a="AttributeValueDeprecatedWarning">
    <sch:p xmlns:ism="urn:us:gov:ic:ism" ism:classification="U" ism:ownerProducer="USA" class="ruleText">
    	[ISM-ID-00198][Warning] Attribute @ism:releasableTo should not contain any  values which will be deprecated.
    </sch:p>
    <sch:p xmlns:ism="urn:us:gov:ic:ism" ism:classification="U" ism:ownerProducer="USA" class="codeDesc">
      For each element which specifies attribute @ism:releasableTo,
      this rule ensures that the value of @ism:releasableTo has not been deprecated. 
      This is indicated in the CVE file by an attribute (@deprecated) 
      on the term element for that releasableTo value. 
      If the current date is less than or equal to the date value in (@deprecated), 
      then a deprecation warning will be given.
    </sch:p>
      <sch:param name="ruleId" value="'ISM-ID-00198'"/>
	<sch:param name="context" value="*[@ism:releasableTo]"/>
	<sch:param name="attrName" value="releasableTo"/>
	<sch:param name="cveName" value="CVEnumISMCATRelTo"/>
	<sch:param name="cveSpec" value="ISMCAT"/>
</sch:pattern>