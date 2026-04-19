<?xml version="1.0" encoding="UTF-8"?>
<?ICEA pattern?>
<?schematron-phases phaseids="ROLLUP VALUECHECK"?>
<!-- Notices - Distribution Notice: 
           This document has been approved for Public Release and is available for use without restriction.
       -->
<sch:pattern xmlns:sch="http://purl.oclc.org/dsdl/schematron" id="ISM-ID-00318" is-a="CheckCommonCountries">
   <sch:p xmlns:ism="urn:us:gov:ic:ism" ism:classification="U" ism:ownerProducer="USA" class="ruleText">
      [ISM-ID-00318][Error] Rollup compilation does not meet CAPCO guidance.
   </sch:p>
   <sch:p xmlns:ism="urn:us:gov:ic:ism" ism:classification="U" ism:ownerProducer="USA" class="codeDesc">
      Where an element is the resource element and contains the @ism:releasableTo attribute,
      check that the values specified meet minimum roll-up conditions. Check all contributing
      portions against the banner for the existence of common countries ensuring that the 
      countries in the banner are the intersection of all contributing portions. Any tetragraphs
      whose decomposable flag is true will be decomposed into their representative countries.
      
      Once the minimum possibility of intersecting countries is determined, the rule checks that  
      all portions of the banner are included in the subset.  The rule then checks for the case where 
      there are no common countries to be rolled up to the resource element.  Finally, the rule checks to
      ensure that if the banner countries are a subset of the common countries, that a
      compilationReason is specified.  If a compilationReason is not specified, then the banner
      releasableTo countries must be the set of common countries from all contributing portions.      
   </sch:p>
   <sch:param name="ruleId" value="'ISM-ID-00318'"/>
   <sch:param name="attrLocalName" value="'releasableTo'"/>
   <sch:param name="actualBannerTokens" value="$relToActualBannerTokens"/> 
   <sch:param name="calculatedBannerTokens" value="$relToCalculatedBannerTokens"/>
</sch:pattern>