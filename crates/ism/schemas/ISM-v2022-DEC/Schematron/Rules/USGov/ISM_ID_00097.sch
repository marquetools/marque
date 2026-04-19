<?xml version="1.0" encoding="UTF-8"?>
<?ICEA pattern?>
<?schematron-phases phaseids="BANNER PORTION VALUECHECK"?>
<!-- Notices - Distribution Notice: 
           This document has been approved for Public Release and is available for use without restriction.
       -->
<sch:pattern xmlns:sch="http://purl.oclc.org/dsdl/schematron" id="ISM-ID-00097">
    <sch:p xmlns:ism="urn:us:gov:ic:ism" ism:classification="U" ism:ownerProducer="USA" class="ruleText">
        [ISM-ID-00097][Warning] If ISM_USGOV_RESOURCE and attribute @ism:FGIsourceProtected is 
        specified with a value other than [FGI] then the value(s) must not be discoverable in IC shared spaces.
        
        Human Readable: FGI Protected should rarely if ever be seen outside of an agency's internal systems.    
    </sch:p>
    <sch:p xmlns:ism="urn:us:gov:ic:ism" ism:classification="U" ism:ownerProducer="USA" class="codeDesc">
    	If the document is an ISM_USGOV_RESOURCE, for each element which specifies
    	the attribute @ism:FGIsourceProtected, this rule ensures that attribute
    	@ism:FGIsourceProtected contains only the token [FGI].
    </sch:p>
	  <sch:rule id="ISM-ID-00097-R1" context="*[$ISM_USGOV_RESOURCE and @ism:FGIsourceProtected]">
        <sch:assert test="normalize-space(string(./@ism:FGIsourceProtected))='FGI'" flag="warning" role="warning">
            [ISM-ID-00097][Warning] If ISM_USGOV_RESOURCE and attribute @ism:FGIsourceProtected is 
            specified with a value other than [FGI] then the value(s) must not be discoverable in IC shared spaces.
            
            Human Readable: FGI Protected should rarely if ever be seen outside of an agency's internal systems. 
        </sch:assert>
    </sch:rule>
</sch:pattern>