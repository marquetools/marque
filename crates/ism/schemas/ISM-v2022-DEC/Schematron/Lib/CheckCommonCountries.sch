<?xml version="1.0" encoding="UTF-8"?>
<!-- Notices - Distribution Notice: 
           This document has been approved for Public Release and is available for use without restriction.
       -->
<!-- Added 2016-03-31: Added helper abstract function to help in re-write of rules 318 & 320 
     that calculate the common countries against either @ism:releasableTo or @ism:displayOnlyTo 
     in the banner -->
<sch:pattern xmlns:sch="http://purl.oclc.org/dsdl/schematron" abstract="true" id="CheckCommonCountries">
  <sch:p xmlns:ism="urn:us:gov:ic:ism" ism:classification="U" ism:ownerProducer="USA" class="codeDesc">
    Where an element is the resource element and contains either the @ism:releasableTo or 
    @ism:displayOnlyTo attributes, check that the values specified meet minimum roll-up conditions. 
    Check all contributing portions against the banner for the existence of common countries 
    ensuring that the countries in the banner are the intersection of all contributing portions. 
    Any tetragraphs whose decomposable flag is true will be decomposed into their representative countries.
    
    Once the minimum possibility of intersecting countries is determined, the rule checks that  
    all portions of the banner are included in the subset.  The rule then checks for the case where 
    there are no common countries to be rolled up to the resource element.  Finally, the rule checks to
    ensure that if the banner countries are a subset of the common countries, that a
    compilationReason is specified.  If a compilationReason is not specified, then the banner
    displayOnlyTo countries must be the set of common countries from all contributing portions.
  </sch:p>
  <sch:rule id="CheckCommonCountries-R1" context="*[generate-id(.) = generate-id($ISM_RESOURCE_ELEMENT) and @ism:*[local-name() = $attrLocalName]]">
    <sch:assert test="count($calculatedBannerTokens) != 0" flag="error" role="error">[<sch:value-of select="$ruleId"/>][Error] The banner cannot have @ism:<sch:value-of select="$attrLocalName"/> because
      there is no common country in the contributing portions.
    </sch:assert>
    <sch:assert test="if(count($calculatedBannerTokens) != 0 and @ism:compilationReason[normalize-space(.)])        then util:isSubsetOf($actualBannerTokens, $calculatedBannerTokens) else true()" flag="error" role="error">[<sch:value-of select="$ruleId"/>][Error] The banner @ism:<sch:value-of select="$attrLocalName"/> must be a subset of the 
      common countries for contributing portions because @ism:compilationReason is specified. Common countries: [<sch:value-of select="util:join($calculatedBannerTokens)"/>].
    </sch:assert> 
    <sch:assert test="if(count($calculatedBannerTokens) != 0 and not(@ism:compilationReason[normalize-space(.)]))        then util:join(util:sort($calculatedBannerTokens)) = util:join(util:sort($actualBannerTokens)) else true()" flag="error" role="error">[<sch:value-of select="$ruleId"/>][Error] The banner @ism:<sch:value-of select="$attrLocalName"/> must match the set of the common countries for 
      contributing portions because @ism:compilationReason is not specified. Common countries: [<sch:value-of select="util:join($calculatedBannerTokens)"/>].
    </sch:assert>    
  </sch:rule>
</sch:pattern>